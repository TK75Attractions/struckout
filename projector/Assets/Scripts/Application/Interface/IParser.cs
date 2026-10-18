namespace Struckout.Application.Interface
{
    public interface IParser
    {
        T Parse<T>(byte[] bytes);
    }
}