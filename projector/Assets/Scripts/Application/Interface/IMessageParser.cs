namespace Struckout.Application.Interface
{
    public interface IMessageParser<T> 
    {
        T MessageParse(byte[] bytes);
    }
}