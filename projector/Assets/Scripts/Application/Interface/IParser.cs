namespace Struckout.Application
{
    public interface IParser
    {
        T Parse<T>(byte[] bytes);
    }
}